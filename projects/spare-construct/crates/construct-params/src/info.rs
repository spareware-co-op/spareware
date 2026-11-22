//! Project information: the small fixed set of project-wide values that title blocks and exports
//! read, with room for project-defined extras (CON-A2-F2-P5).
//!
//! The values are parameters of the project itself: each built-in field is a [`Definition`] with a
//! permanent ID pinned by a test (see [`InfoField::id_text`]), and its value is a [`Value`] kept under
//! that ID, the same as any element's value, so [`ProjectInfo::to_props`] gives `props` entries.
//! The fields are the project's name, number, client and address, its stage, and its issue date.
//! The stage is text because offices name their stages differently. The issue date is a calendar
//! date the caller gives ([`IssueDate`]); it is kept as text in the one spelling `YYYY-MM-DD`.
//! Extras are definitions the project adds. Everything is pure and returns a new value; nothing
//! reads a clock. The stored form is in FORMAT.md.

use crate::definition::{Definition, Holder, Kind};
use crate::error::ParamError;
use crate::units::Units;
use crate::value::Value;
use crate::values::Values;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// One of the built-in project information fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InfoField {
    /// The project's name.
    Name,
    /// The project's number.
    Number,
    /// The client.
    Client,
    /// The site's address.
    Address,
    /// The stage or status the project is at, in the office's own words.
    Stage,
    /// The date of issue, a calendar date: see [`IssueDate`].
    IssueDate,
}

impl InfoField {
    /// Every field, in the order title blocks and schedules list them.
    pub const ALL: [Self; 6] = [
        Self::Name,
        Self::Number,
        Self::Client,
        Self::Address,
        Self::Stage,
        Self::IssueDate,
    ];

    /// The field's permanent ID. Pinned: stored projects and title block recipes hold these.
    #[must_use]
    pub const fn id_text(self) -> &'static str {
        match self {
            Self::Name => "project-info.name",
            Self::Number => "project-info.number",
            Self::Client => "project-info.client",
            Self::Address => "project-info.address",
            Self::Stage => "project-info.stage",
            Self::IssueDate => "project-info.issue-date",
        }
    }

    /// The name people see.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Project name",
            Self::Number => "Project number",
            Self::Client => "Client",
            Self::Address => "Address",
            Self::Stage => "Stage",
            Self::IssueDate => "Issue date",
        }
    }

    /// The field as a parameter definition: text, kept once for the project. `None` only if the
    /// pinned ID text failed the ID rules, which a test rules out.
    #[must_use]
    pub fn definition(self) -> Option<Definition> {
        let id = Id::parse(self.id_text()).ok()?;
        Definition::new(id, self.label(), Kind::Text, Holder::Instance).ok()
    }

    fn from_id(id: &Id) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id_text() == id.as_str())
    }
}

/// Whether `id` is one of the built-in project information fields.
#[must_use]
pub fn is_builtin_id(id: &Id) -> bool {
    InfoField::from_id(id).is_some()
}

/// A calendar date of issue, checked as a real date and kept as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IssueDate {
    year: u16,
    month: u8,
    day: u8,
}

impl IssueDate {
    /// A date, checked against the calendar (leap years included). Years 1 to 9999.
    ///
    /// # Errors
    /// [`InfoError::BadDate`] if there is no such day.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, InfoError> {
        let bad = || InfoError::BadDate(format!("{year}-{month}-{day}"));
        if year == 0 || year > 9999 {
            return Err(bad());
        }
        let month_of = time::Month::try_from(month).map_err(|_| bad())?;
        time::Date::from_calendar_date(i32::from(year), month_of, day).map_err(|_| bad())?;
        Ok(Self { year, month, day })
    }

    /// Reads `YYYY-MM-DD`, four, two and two digits.
    ///
    /// # Errors
    /// [`InfoError::BadDate`] if the text is not in that form or is not a real day.
    pub fn parse(text: &str) -> Result<Self, InfoError> {
        let bad = || InfoError::BadDate(text.to_owned());
        let mut parts = text.split('-');
        let (Some(y), Some(m), Some(d), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        let digits = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(y, 4) || !digits(m, 2) || !digits(d, 2) {
            return Err(bad());
        }
        Self::new(
            y.parse().map_err(|_| bad())?,
            m.parse().map_err(|_| bad())?,
            d.parse().map_err(|_| bad())?,
        )
    }

    /// The year.
    #[must_use]
    pub fn year(self) -> u16 {
        self.year
    }

    /// The month, 1 to 12.
    #[must_use]
    pub fn month(self) -> u8 {
        self.month
    }

    /// The day of the month.
    #[must_use]
    pub fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for IssueDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Why project information or a stored form of it was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InfoError {
    /// There is no such calendar date.
    BadDate(String),
    /// The field takes a date, not text (or the reverse): use the function for its sort.
    WrongField(InfoField),
    /// The ID is a built-in field's, and an extra cannot take it.
    ReservedId(Id),
    /// The ID is already an extra that differs.
    Conflict(Id),
    /// Another field or extra has this name (case does not matter).
    DuplicateName(String),
    /// The project information has no extra with this ID.
    UnknownExtra(Id),
    /// A value was refused by the parameter it is for.
    Param(ParamError),
    /// The stored form is not a version this build reads.
    UnsupportedFormat(u32),
    /// The stored form is not valid; the text says why.
    Malformed(String),
}

impl fmt::Display for InfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadDate(text) => write!(f, "{text:?} is not a calendar date"),
            Self::WrongField(field) => write!(f, "{} takes another sort of value", field.label()),
            Self::ReservedId(id) => write!(f, "{} is a built-in project field", id.as_str()),
            Self::Conflict(id) => write!(f, "{} is already a different extra", id.as_str()),
            Self::DuplicateName(name) => write!(f, "project information already has {name:?}"),
            Self::UnknownExtra(id) => write!(f, "no extra has id {}", id.as_str()),
            Self::Param(error) => error.fmt(f),
            Self::UnsupportedFormat(v) => {
                write!(f, "project information format {v} is not supported")
            }
            Self::Malformed(why) => write!(f, "project information is not valid: {why}"),
        }
    }
}

impl std::error::Error for InfoError {}

impl From<ParamError> for InfoError {
    fn from(error: ParamError) -> Self {
        Self::Param(error)
    }
}

/// One line of project information as a title block reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoRow {
    /// The parameter's permanent ID.
    pub id: Id,
    /// The name people see.
    pub label: String,
    /// The value shown as the project's units say; `None` if it has none yet.
    pub text: Option<String>,
}

/// A project's information: the built-in fields' values, and any extras the project defined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectInfo {
    values: Values,
    extras: BTreeMap<Id, Definition>,
}

const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: u32,
    extras: Vec<Definition>,
    values: BTreeMap<String, String>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl ProjectInfo {
    /// Project information with no values and no extras.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn definitions(&self) -> Vec<Definition> {
        InfoField::ALL
            .into_iter()
            .filter_map(InfoField::definition)
            .chain(self.extras.values().cloned())
            .collect()
    }

    fn with_value(&self, definition: &Definition, value: Value) -> Result<Self, InfoError> {
        let mut next = self.clone();
        next.values.set(definition, value)?;
        Ok(next)
    }

    fn with_field(&self, field: InfoField, value: Value) -> Result<Self, InfoError> {
        let definition = field
            .definition()
            .ok_or_else(|| InfoError::Malformed(format!("{} has no definition", field.label())))?;
        self.with_value(&definition, value)
    }

    fn value_of(&self, field: InfoField) -> Option<&Value> {
        self.values
            .iter()
            .find(|(id, _)| id.as_str() == field.id_text())
            .map(|(_, value)| value)
    }

    /// The text of a text field; `None` if it has none, or if the field is the issue date.
    #[must_use]
    pub fn text(&self, field: InfoField) -> Option<&str> {
        match self.value_of(field) {
            Some(Value::Text(text)) if field != InfoField::IssueDate => Some(text),
            _ => None,
        }
    }

    /// The issue date; `None` if it has none.
    #[must_use]
    pub fn issue_date(&self) -> Option<IssueDate> {
        match self.value_of(InfoField::IssueDate) {
            Some(Value::Text(text)) => IssueDate::parse(text).ok(),
            _ => None,
        }
    }

    /// The information with a text field set.
    ///
    /// # Errors
    /// [`InfoError::WrongField`] for the issue date, which takes [`ProjectInfo::set_issue_date`].
    pub fn set_text(&self, field: InfoField, text: &str) -> Result<Self, InfoError> {
        if field == InfoField::IssueDate {
            return Err(InfoError::WrongField(field));
        }
        self.with_field(field, Value::Text(text.to_owned()))
    }

    /// The information with the issue date set.
    ///
    /// # Errors
    /// [`InfoError::Param`] only if the stored form cannot be made, which a checked date never hits.
    pub fn set_issue_date(&self, date: IssueDate) -> Result<Self, InfoError> {
        self.with_field(InfoField::IssueDate, Value::Text(date.to_string()))
    }

    /// The information with a built-in field's value taken away.
    #[must_use]
    pub fn clear(&self, field: InfoField) -> Self {
        let mut next = self.clone();
        let stored = next
            .values
            .iter()
            .find(|(id, _)| id.as_str() == field.id_text())
            .map(|(id, _)| id.clone());
        if let Some(id) = stored {
            next.values.remove(&id);
        }
        next
    }

    /// The information with an extra defined. Defining the same extra again changes nothing.
    ///
    /// # Errors
    /// [`InfoError::ReservedId`] for a built-in field's ID, [`InfoError::Conflict`] if the ID is a
    /// different extra, or [`InfoError::DuplicateName`].
    pub fn add_extra(&self, definition: Definition) -> Result<Self, InfoError> {
        if is_builtin_id(definition.id()) {
            return Err(InfoError::ReservedId(definition.id().clone()));
        }
        if let Some(existing) = self.extras.get(definition.id()) {
            return if *existing == definition {
                Ok(self.clone())
            } else {
                Err(InfoError::Conflict(definition.id().clone()))
            };
        }
        if self
            .definitions()
            .iter()
            .any(|other| same_name(other.name(), definition.name()))
        {
            return Err(InfoError::DuplicateName(definition.name().to_owned()));
        }
        let mut next = self.clone();
        next.extras.insert(definition.id().clone(), definition);
        Ok(next)
    }

    /// The information with an extra's value set.
    ///
    /// # Errors
    /// [`InfoError::UnknownExtra`], or [`InfoError::Param`] if the value is not one the extra takes.
    pub fn set_extra(&self, id: &Id, value: Value) -> Result<Self, InfoError> {
        let definition = self
            .extras
            .get(id)
            .ok_or_else(|| InfoError::UnknownExtra(id.clone()))?;
        self.with_value(definition, value)
    }

    /// An extra's value.
    #[must_use]
    pub fn extra(&self, id: &Id) -> Option<&Value> {
        self.extras.get(id).and(self.values.get(id))
    }

    /// The extras' definitions, in ID order.
    pub fn extras(&self) -> impl Iterator<Item = &Definition> {
        self.extras.values()
    }

    /// Every field and extra for a title block: the built-in fields in their order, then the
    /// extras by ID, each with its value shown as `units` says.
    #[must_use]
    pub fn rows(&self, units: &Units) -> Vec<InfoRow> {
        self.definitions()
            .into_iter()
            .map(|definition| {
                let text = self
                    .values
                    .get(definition.id())
                    .and_then(|value| definition.format(value, units).ok());
                InfoRow {
                    id: definition.id().clone(),
                    label: definition.name().to_owned(),
                    text,
                }
            })
            .collect()
    }

    /// The values as `props` entries, parameter ID to stored text.
    #[must_use]
    pub fn to_props(&self) -> BTreeMap<String, String> {
        self.values.to_props()
    }

    /// The stored bytes: the same information always gives the same bytes (FORMAT.md).
    ///
    /// # Errors
    /// [`InfoError::Malformed`] if the text cannot be written, which valid information never hits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, InfoError> {
        let wire = Wire {
            format: FORMAT,
            extras: self.extras.values().cloned().collect(),
            values: self.to_props(),
        };
        serde_json::to_vec(&wire).map_err(|e| InfoError::Malformed(e.to_string()))
    }

    /// Reads stored information. Every rule is checked again: each value must be of a defined
    /// parameter, the issue date a real day, and the bytes the one spelling
    /// [`ProjectInfo::to_bytes`] writes.
    ///
    /// # Errors
    /// [`InfoError::UnsupportedFormat`] for a format this build does not know, otherwise
    /// [`InfoError::Malformed`] or the rule that an entry broke.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InfoError> {
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|e| InfoError::Malformed(e.to_string()))?;
        if wire.format != FORMAT {
            return Err(InfoError::UnsupportedFormat(wire.format));
        }
        let mut info = Self::new();
        for definition in wire.extras {
            info = info.add_extra(definition)?;
        }
        let defined = info.definitions();
        if let Some(stray) = wire
            .values
            .keys()
            .find(|key| !defined.iter().any(|d| d.id().as_str() == key.as_str()))
        {
            return Err(InfoError::Malformed(format!("{stray} is not a field")));
        }
        info.values = Values::from_props(defined.iter(), &wire.values)?;
        if let Some(Value::Text(text)) = info.value_of(InfoField::IssueDate) {
            IssueDate::parse(text)?;
        }
        if info.to_bytes()? != bytes {
            return Err(InfoError::Malformed(
                "not in its one spelling (order, repeats, spacing or escapes)".to_owned(),
            ));
        }
        Ok(info)
    }
}
