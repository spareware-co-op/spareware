//! One text note: the lines of text, where it sits in its view, how it wraps and aligns, its
//! rotation, the style it reads and its leaders, written as an element record of the text notes
//! category and read back. Nothing about how the text is laid out is stored here: that is worked
//! from the record, the style and font metrics each time ([`crate::lay_out`]).

use crate::error::TextError;
use crate::stored;
use construct_categories::TEXT_NOTES;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Angle, Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every text note record, and its recipe's operation.
pub const NOTE_KIND: &str = "text-note";
/// The name every text note record carries.
pub const NOTE_NAME: &str = "Text note";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The view the note belongs to, an ID.
pub const VIEW_KEY: &str = "@text.view";
/// The text style the note reads, an ID.
pub const STYLE_KEY: &str = "@text.style";
/// The note's anchor across the view, a stored length in the view's own coordinates.
pub const X_KEY: &str = "@text.x";
/// The note's anchor up the view, a stored length.
pub const Y_KEY: &str = "@text.y";
/// The lines of text, joined by a line feed.
pub const TEXT_KEY: &str = "@text.lines";
/// The wrap width, a stored length above zero. Absent when the note does not wrap.
pub const WIDTH_KEY: &str = "@text.width";
/// The alignment, in construct-params' stored choice text: `left`, `centre` or `right`.
pub const ALIGN_KEY: &str = "@text.align";
/// The list style, in the same text: `none`, `bullet` or `number`.
pub const LIST_KEY: &str = "@text.list";
/// The rotation about the anchor, a stored angle.
pub const ROTATION_KEY: &str = "@text.rotation";
/// What every leader's key begins with; the leader's number follows.
pub const LEADER_PREFIX: &str = "@text.leader.";

/// The most characters a note may hold.
pub const MAX_CHARS: usize = 10_000;
/// The most points a leader may have.
pub const MAX_LEADER_POINTS: usize = 32;
/// The most leaders a note may have.
pub const MAX_LEADERS: usize = 32;
/// One turn, in angle units: a rotation is within a turn either way.
pub const TURN: i64 = Angle::UNITS_PER_DEGREE * 360;

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a note record: Spare Construct's own operation `text-note`, version 1, with no
/// arguments. It builds no shape.
#[must_use]
pub fn note_recipe() -> Recipe {
    Recipe::construct(NOTE_KIND, BTreeMap::new())
}

/// How each line sits in the note's box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Align {
    /// Against the left edge.
    Left,
    /// Centred.
    Centre,
    /// Against the right edge.
    Right,
}

impl Align {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Centre => "centre",
            Self::Right => "right",
        }
    }

    /// The alignment a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "left" => Some(Self::Left),
            "centre" => Some(Self::Centre),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// What each line of text is: a plain paragraph, a bullet or a numbered item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListKind {
    /// A plain paragraph.
    None,
    /// Each line is a bulleted item.
    Bullet,
    /// Each line is a numbered item, counted from 1.
    Number,
}

impl ListKind {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bullet => "bullet",
            Self::Number => "number",
        }
    }

    /// The list kind a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "none" => Some(Self::None),
            "bullet" => Some(Self::Bullet),
            "number" => Some(Self::Number),
            _ => None,
        }
    }
}

/// The arrowhead at a leader's last point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arrow {
    /// No head.
    None,
    /// Two open strokes.
    Open,
    /// A filled triangle.
    Filled,
    /// A filled dot.
    Dot,
    /// A short slash across the line.
    Slash,
}

impl Arrow {
    /// Every kind.
    pub const ALL: [Self; 5] = [Self::None, Self::Open, Self::Filled, Self::Dot, Self::Slash];

    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Open => "open",
            Self::Filled => "filled",
            Self::Dot => "dot",
            Self::Slash => "slash",
        }
    }

    /// The kind a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}

/// A leader: a line through two or more points in the view's own coordinates, ending at an
/// arrowhead on the last point. It is not tied to an element: it is where it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leader {
    points: Vec<(Length, Length)>,
    arrow: Arrow,
}

impl Leader {
    /// A leader through `points` ending at `arrow`.
    ///
    /// # Errors
    /// [`TextError::BadLeader`] for fewer than two points or more than [`MAX_LEADER_POINTS`].
    pub fn new(points: Vec<(Length, Length)>, arrow: Arrow) -> Result<Self, TextError> {
        if points.len() < 2 || points.len() > MAX_LEADER_POINTS {
            return Err(TextError::BadLeader(format!(
                "A leader has from 2 to {MAX_LEADER_POINTS} points; this one has {}.",
                points.len()
            )));
        }
        Ok(Self { points, arrow })
    }

    /// The points, from the note to the arrowhead.
    #[must_use]
    pub fn points(&self) -> &[(Length, Length)] {
        &self.points
    }

    /// The arrowhead kind at the last point.
    #[must_use]
    pub fn arrow(&self) -> Arrow {
        self.arrow
    }

    fn stored(&self) -> String {
        let mut text = self.arrow.key().to_owned();
        for (x, y) in &self.points {
            text.push(';');
            text.push_str(&Value::Length(*x).stored());
            text.push(',');
            text.push_str(&Value::Length(*y).stored());
        }
        text
    }

    fn from_stored(text: &str) -> Option<Self> {
        let mut parts = text.split(';');
        let arrow = Arrow::from_key(parts.next()?)?;
        let mut points = Vec::new();
        for part in parts {
            let (x, y) = part.split_once(',')?;
            let point = |text: &str| match Value::from_stored(text)? {
                Value::Length(length) => Some(length),
                _ => None,
            };
            points.push((point(x)?, point(y)?));
        }
        Self::new(points, arrow).ok()
    }
}

/// Text as kept: line feeds only, tabs allowed, no other control character, not empty, not long.
fn clean_text(text: &str) -> Result<Vec<String>, TextError> {
    let text = text.replace("\r\n", "\n");
    let text = text.trim_end_matches('\n');
    if text.trim().is_empty() {
        return Err(TextError::BadText(
            "A text note needs some text.".to_owned(),
        ));
    }
    if text.chars().count() > MAX_CHARS
        || text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(TextError::BadText(format!(
            "A text note holds at most {MAX_CHARS} characters, with line feeds and tabs the only \
             control characters."
        )));
    }
    Ok(text.split('\n').map(str::to_owned).collect())
}

/// A text note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextNote {
    id: Id,
    view: Id,
    style: Id,
    x: Length,
    y: Length,
    lines: Vec<String>,
    width: Option<Length>,
    align: Align,
    list: ListKind,
    rotation: Angle,
    leaders: BTreeMap<u32, Leader>,
}

impl TextNote {
    /// A note in `view` at (`x`, `y`) with `text` (lines separated by line feeds) in `style`: not
    /// wrapped, left aligned, not a list, not rotated, with no leader.
    ///
    /// # Errors
    /// [`TextError::BadText`] for text that is empty, long or holds a control character.
    pub fn new(
        id: Id,
        view: Id,
        style: Id,
        position: (Length, Length),
        text: &str,
    ) -> Result<Self, TextError> {
        Ok(Self {
            id,
            view,
            style,
            x: position.0,
            y: position.1,
            lines: clean_text(text)?,
            width: None,
            align: Align::Left,
            list: ListKind::None,
            rotation: Angle::from_units(0),
            leaders: BTreeMap::new(),
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it belongs to.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// The style it reads.
    #[must_use]
    pub fn style(&self) -> &Id {
        &self.style
    }

    /// The anchor across the view: the left edge of the box.
    #[must_use]
    pub fn x(&self) -> Length {
        self.x
    }

    /// The anchor up the view: the top edge of the box.
    #[must_use]
    pub fn y(&self) -> Length {
        self.y
    }

    /// The lines of text, each one a paragraph that wraps when the note has a width.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The text as one string, lines joined by line feeds.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// The wrap width, if the note wraps.
    #[must_use]
    pub fn width(&self) -> Option<Length> {
        self.width
    }

    /// How each line sits in the box.
    #[must_use]
    pub fn align(&self) -> Align {
        self.align
    }

    /// Whether the lines are a list.
    #[must_use]
    pub fn list(&self) -> ListKind {
        self.list
    }

    /// The rotation about the anchor, anticlockwise.
    #[must_use]
    pub fn rotation(&self) -> Angle {
        self.rotation
    }

    /// The leaders by number, the numbers kept as they were made.
    #[must_use]
    pub fn leaders(&self) -> &BTreeMap<u32, Leader> {
        &self.leaders
    }

    /// The note with other text.
    ///
    /// # Errors
    /// [`TextError::BadText`].
    pub fn with_text(mut self, text: &str) -> Result<Self, TextError> {
        self.lines = clean_text(text)?;
        Ok(self)
    }

    /// The note anchored somewhere else in its view.
    #[must_use]
    pub fn at(mut self, x: Length, y: Length) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// The note reading another style. That the style is there is [`crate::Texts`]' rule.
    #[must_use]
    pub fn with_style(mut self, style: Id) -> Self {
        self.style = style;
        self
    }

    /// The note wrapping at a width, or not wrapping.
    ///
    /// # Errors
    /// [`TextError::BadSize`] for a width that is not above zero.
    pub fn with_width(mut self, width: Option<Length>) -> Result<Self, TextError> {
        if let Some(width) = width
            && width <= Length::from_steps(0)
        {
            return Err(TextError::BadSize(format!(
                "A wrap width is above zero; this is {width}."
            )));
        }
        self.width = width;
        Ok(self)
    }

    /// The note aligned another way.
    #[must_use]
    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// The note with another list kind.
    #[must_use]
    pub fn with_list(mut self, list: ListKind) -> Self {
        self.list = list;
        self
    }

    /// The note rotated.
    ///
    /// # Errors
    /// [`TextError::BadRotation`] for a rotation of more than one turn either way.
    pub fn with_rotation(mut self, rotation: Angle) -> Result<Self, TextError> {
        if rotation
            .units()
            .checked_abs()
            .is_none_or(|units| units > TURN)
        {
            return Err(TextError::BadRotation(
                "A rotation is at most one turn (360 degrees) either way.".to_owned(),
            ));
        }
        self.rotation = rotation;
        Ok(self)
    }

    /// The note with a leader added, and the leader's number: one more than the largest the note
    /// has now.
    ///
    /// # Errors
    /// [`TextError::BadLeader`] when the note has [`MAX_LEADERS`] already.
    pub fn with_leader(mut self, leader: Leader) -> Result<(Self, u32), TextError> {
        if self.leaders.len() >= MAX_LEADERS {
            return Err(TextError::BadLeader(format!(
                "A text note has at most {MAX_LEADERS} leaders."
            )));
        }
        let number = self
            .leaders
            .keys()
            .next_back()
            .map_or(Some(1), |last| last.checked_add(1))
            .ok_or_else(|| TextError::OutOfRange("leader numbers".to_owned()))?;
        self.leaders.insert(number, leader);
        Ok((self, number))
    }

    /// The note with leader `number` replaced.
    ///
    /// # Errors
    /// [`TextError::UnknownLeader`].
    pub fn with_leader_replaced(mut self, number: u32, leader: Leader) -> Result<Self, TextError> {
        match self.leaders.get_mut(&number) {
            Some(slot) => *slot = leader,
            None => {
                return Err(TextError::UnknownLeader {
                    note: self.id,
                    leader: number,
                });
            }
        }
        Ok(self)
    }

    /// The note without leader `number`.
    ///
    /// # Errors
    /// [`TextError::UnknownLeader`].
    pub fn without_leader(mut self, number: u32) -> Result<Self, TextError> {
        if self.leaders.remove(&number).is_none() {
            return Err(TextError::UnknownLeader {
                note: self.id,
                leader: number,
            });
        }
        Ok(self)
    }

    /// The note as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), TEXT_NOTES.0.to_string());
        props.insert(VIEW_KEY.to_owned(), self.view.to_string());
        props.insert(STYLE_KEY.to_owned(), self.style.to_string());
        props.insert(X_KEY.to_owned(), Value::Length(self.x).stored());
        props.insert(Y_KEY.to_owned(), Value::Length(self.y).stored());
        props.insert(TEXT_KEY.to_owned(), self.text());
        if let Some(width) = self.width {
            props.insert(WIDTH_KEY.to_owned(), Value::Length(width).stored());
        }
        props.insert(
            ALIGN_KEY.to_owned(),
            Value::Choice(self.align.key().to_owned()).stored(),
        );
        props.insert(
            LIST_KEY.to_owned(),
            Value::Choice(self.list.key().to_owned()).stored(),
        );
        props.insert(
            ROTATION_KEY.to_owned(),
            Value::Angle(self.rotation).stored(),
        );
        for (number, leader) in &self.leaders {
            props.insert(format!("{LEADER_PREFIX}{number}"), leader.stored());
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: NOTE_KIND.to_owned(),
            name: NOTE_NAME.to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: note_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a note from its element record. Anything [`TextNote::to_record`] would not write is
    /// refused, never repaired.
    ///
    /// # Errors
    /// [`TextError::Malformed`], naming the record, for a record that is not a note record as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, TextError> {
        let malformed =
            |why: &str| TextError::Malformed(format!("The text note record {} {why}.", record.id));
        if record.kind != NOTE_KIND || record.recipe != note_recipe() {
            return Err(malformed(
                "is not of kind text-note, with the text note recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let id_at = |key: &str| {
            record
                .props
                .get(key)
                .and_then(|text| Id::parse(text).ok())
                .ok_or_else(|| malformed(&format!("holds no ID under {key}")))
        };
        let (Some(x), Some(y)) = (stored::length(record, X_KEY), stored::length(record, Y_KEY))
        else {
            return Err(malformed("holds no position in its one spelling"));
        };
        let text = record
            .props
            .get(TEXT_KEY)
            .ok_or_else(|| malformed("holds no text"))?;
        let choice = |key: &str| match stored::value(record, key) {
            Some(Value::Choice(key)) => Ok(key),
            _ => Err(malformed(&format!("holds no choice under {key}"))),
        };
        let Some(Value::Angle(rotation)) = stored::value(record, ROTATION_KEY) else {
            return Err(malformed("holds no rotation in its one spelling"));
        };
        let width = if record.props.contains_key(WIDTH_KEY) {
            Some(
                stored::length(record, WIDTH_KEY)
                    .ok_or_else(|| malformed("holds a width that is not in its one spelling"))?,
            )
        } else {
            None
        };
        let mut note = Self::new(
            record.id.clone(),
            id_at(VIEW_KEY)?,
            id_at(STYLE_KEY)?,
            (x, y),
            text,
        )
        .and_then(|note| note.with_width(width))
        .and_then(|note| note.with_rotation(rotation))
        .map_err(|e| malformed(&e.to_string()))?
        .with_align(
            Align::from_key(&choice(ALIGN_KEY)?)
                .ok_or_else(|| malformed("holds an alignment that is not left, centre or right"))?,
        )
        .with_list(
            ListKind::from_key(&choice(LIST_KEY)?)
                .ok_or_else(|| malformed("holds a list kind that is not none, bullet or number"))?,
        );
        for (key, value) in &record.props {
            let Some(number) = key.strip_prefix(LEADER_PREFIX) else {
                continue;
            };
            let number: u32 = number
                .parse()
                .map_err(|_| malformed("holds a leader with no number"))?;
            let leader = Leader::from_stored(value)
                .ok_or_else(|| malformed("holds a leader that is not as written"))?;
            note.leaders.insert(number, leader);
        }
        if note.to_record(&record.document) != *record {
            return Err(malformed("is not as a text note record is written"));
        }
        Ok(note)
    }
}
