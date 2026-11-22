//! What a parameter is: its permanent ID, its name, the kind of value it takes, and whether it
//! belongs to each instance or to the type all of them share.

use crate::error::ParamError;
use crate::text;
use crate::units::Units;
use crate::value::Value;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The kind of value a parameter takes. The serialised name is also the tag of a stored value
/// (FORMAT.md), so neither may ever change.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A length, kept in whole millimetres.
    Length,
    /// An area, kept in whole square millimetres.
    Area,
    /// A volume, kept in whole cubic millimetres.
    Volume,
    /// An angle, kept in whole angle units of the geometry kernel.
    Angle,
    /// A slope: a rise over a run, kept as a reduced ratio of whole numbers.
    Slope,
    /// A number with no unit, kept as an exact decimal.
    Number,
    /// A whole number with no unit.
    Whole,
    /// Text.
    Text,
    /// Yes or no.
    YesNo,
    /// One of a list of choices the definition names.
    Choice,
    /// A link to another element, by its permanent ID.
    Element,
    /// A material, by its permanent ID.
    Material,
    /// A web address.
    Link,
}

impl Kind {
    /// Every kind, in a fixed order.
    pub const ALL: [Self; 13] = [
        Self::Length,
        Self::Area,
        Self::Volume,
        Self::Angle,
        Self::Slope,
        Self::Number,
        Self::Whole,
        Self::Text,
        Self::YesNo,
        Self::Choice,
        Self::Element,
        Self::Material,
        Self::Link,
    ];

    /// The kind's tag in stored text, the same as its serialised name.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Area => "area",
            Self::Volume => "volume",
            Self::Angle => "angle",
            Self::Slope => "slope",
            Self::Number => "number",
            Self::Whole => "whole",
            Self::Text => "text",
            Self::YesNo => "yesno",
            Self::Choice => "choice",
            Self::Element => "element",
            Self::Material => "material",
            Self::Link => "link",
        }
    }

    /// The kind a stored tag names.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.tag() == tag)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "a length",
            Self::Area => "an area",
            Self::Volume => "a volume",
            Self::Angle => "an angle",
            Self::Slope => "a slope",
            Self::Number => "a number",
            Self::Whole => "a whole number",
            Self::Text => "text",
            Self::YesNo => "yes or no",
            Self::Choice => "a choice",
            Self::Element => "an element link",
            Self::Material => "a material",
            Self::Link => "a web link",
        })
    }
}

/// Whether a parameter's value is kept on each instance or once on the type.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Holder {
    /// Each element has its own value.
    Instance,
    /// The type holds one value for every element of that type.
    Type,
}

/// One entry in a choice parameter's list. The key is what is stored, so a label can be reworded
/// without changing any stored value.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// What is stored: never empty, never changed once used.
    pub key: String,
    /// What people see and type.
    pub label: String,
}

impl Choice {
    /// A choice from its key and label.
    #[must_use]
    pub fn new(key: &str, label: &str) -> Self {
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
        }
    }
}

/// A parameter: what every value of it must be. Made only through the checks below, as made and
/// as read back, so a definition in hand always keeps its rules.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(try_from = "Row")]
pub struct Definition {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
    kind: Kind,
    holder: Holder,
    choices: Vec<Choice>,
}

/// A definition as read, before its checks.
#[derive(Deserialize)]
struct Row {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
    kind: Kind,
    holder: Holder,
    choices: Vec<Choice>,
}

impl TryFrom<Row> for Definition {
    type Error = ParamError;

    fn try_from(row: Row) -> Result<Self, ParamError> {
        let definition = Self {
            id: row.id,
            name: row.name,
            kind: row.kind,
            holder: row.holder,
            choices: row.choices,
        };
        definition.rules()?;
        Ok(definition)
    }
}

impl Definition {
    /// A parameter of any kind but a choice, which needs its list: see [`Definition::choice`].
    ///
    /// # Errors
    /// [`ParamError::BadDefinition`] if the name is empty or the kind is a choice.
    pub fn new(id: Id, name: &str, kind: Kind, holder: Holder) -> Result<Self, ParamError> {
        let definition = Self {
            id,
            name: name.to_owned(),
            kind,
            holder,
            choices: Vec::new(),
        };
        definition.rules()?;
        Ok(definition)
    }

    /// A choice parameter with its list, in the order people see it.
    ///
    /// # Errors
    /// [`ParamError::BadDefinition`] if the name is empty, the list is empty, a key is empty, or
    /// two choices share a key or a label.
    pub fn choice(
        id: Id,
        name: &str,
        holder: Holder,
        choices: Vec<Choice>,
    ) -> Result<Self, ParamError> {
        let definition = Self {
            id,
            name: name.to_owned(),
            kind: Kind::Choice,
            holder,
            choices,
        };
        definition.rules()?;
        Ok(definition)
    }

    fn rules(&self) -> Result<(), ParamError> {
        let bad = |reason| {
            Err(ParamError::BadDefinition {
                parameter: self.name.clone(),
                reason,
            })
        };
        if self.name.is_empty() {
            return bad("a parameter needs a name");
        }
        if self.kind != Kind::Choice {
            if !self.choices.is_empty() {
                return bad("only a choice parameter has a list of choices");
            }
            return Ok(());
        }
        if self.choices.is_empty() {
            return bad("a choice parameter needs at least one choice");
        }
        for (at, choice) in self.choices.iter().enumerate() {
            if choice.key.is_empty() {
                return bad("a choice needs a key");
            }
            let earlier = self.choices.iter().take(at);
            if earlier
                .clone()
                .any(|other| other.key == choice.key || other.label == choice.label)
            {
                return bad("two choices share a key or a label");
            }
        }
        Ok(())
    }

    /// Its permanent ID: what values are stored under.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The name people see.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The kind of value it takes.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Whether its value is on each instance or on the type.
    #[must_use]
    pub fn holder(&self) -> Holder {
        self.holder
    }

    /// A choice parameter's list; empty for any other kind.
    #[must_use]
    pub fn choices(&self) -> &[Choice] {
        &self.choices
    }

    /// The definition as a JSON row, its fields in one fixed order.
    ///
    /// # Errors
    /// [`ParamError::BadDefinition`] if the row could not be written.
    pub fn to_json(&self) -> Result<String, ParamError> {
        serde_json::to_string(self).map_err(|_| ParamError::BadDefinition {
            parameter: self.name.clone(),
            reason: "the definition could not be written",
        })
    }

    /// Reads a definition from its JSON row, with the same checks as making one.
    ///
    /// # Errors
    /// [`ParamError::BadDefinition`] if the row is not a definition or breaks a rule.
    pub fn from_json(json: &str) -> Result<Self, ParamError> {
        let row: Row = serde_json::from_str(json).map_err(|_| ParamError::BadDefinition {
            parameter: String::new(),
            reason: "the row is not a parameter definition",
        })?;
        Self::try_from(row)
    }

    /// Whether `value` is one this parameter takes: its kind, and for a choice, one in the list.
    ///
    /// # Errors
    /// [`ParamError::WrongKind`] naming this parameter and both kinds, or
    /// [`ParamError::NotAChoice`].
    pub fn check(&self, value: &Value) -> Result<(), ParamError> {
        if value.kind() != self.kind {
            return Err(ParamError::WrongKind {
                parameter: self.name.clone(),
                expected: self.kind,
                found: value.kind(),
            });
        }
        if let Value::Choice(key) = value
            && !self.choices.iter().any(|choice| &choice.key == key)
        {
            return Err(ParamError::NotAChoice {
                parameter: self.name.clone(),
                text: key.clone(),
            });
        }
        Ok(())
    }

    /// The stored text of a value this parameter takes.
    ///
    /// # Errors
    /// As [`Definition::check`].
    pub fn write(&self, value: &Value) -> Result<String, ParamError> {
        self.check(value)?;
        Ok(value.stored())
    }

    /// Reads a value of this parameter from its stored text.
    ///
    /// # Errors
    /// [`ParamError::BadStored`] if the text is not a stored value in its one spelling, or as
    /// [`Definition::check`].
    pub fn read(&self, stored: &str) -> Result<Value, ParamError> {
        let value = Value::from_stored(stored).ok_or_else(|| ParamError::BadStored {
            parameter: self.name.clone(),
            text: stored.to_owned(),
        })?;
        self.check(&value)?;
        Ok(value)
    }

    /// Reads text a person typed, such as `2400`, `2.4 m` or `2400mm` for a length. A number with
    /// no unit is in the unit `units` shows this kind in.
    ///
    /// # Errors
    /// A [`ParamError`] naming this parameter: the wrong unit, an unknown unit, no number, a
    /// fraction for a whole number, a value out of range, or a choice, ID or link that is not one.
    pub fn parse(&self, typed: &str, units: &Units) -> Result<Value, ParamError> {
        let value = text::parse(self, typed, units)?;
        self.check(&value)?;
        Ok(value)
    }

    /// Shows a value of this parameter as `units` says: `2400 mm`, `12.50 m²`, a choice's label.
    ///
    /// # Errors
    /// As [`Definition::check`].
    pub fn format(&self, value: &Value, units: &Units) -> Result<String, ParamError> {
        self.check(value)?;
        text::format(self, value, units)
    }
}
