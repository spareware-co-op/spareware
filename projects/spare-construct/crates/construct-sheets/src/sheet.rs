//! One sheet: its permanent ID, number, name and paper, written as an element record of the sheets
//! category and read back. Its parameters (drawn by, checked by...) are not here: they are
//! construct-params' own, held in the record's `props` by parameter ID like any element's.

use crate::error::SheetError;
use crate::paper::{Orientation, Paper, PaperSize};
use construct_categories::SHEETS;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every sheet record, and its recipe's operation.
pub const SHEET_KIND: &str = "sheet";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The sheet's number, as text.
pub const NUMBER_KEY: &str = "@sheet.number";
/// The paper: a size's key (`a3`) or `custom`, in construct-params' stored choice text.
pub const PAPER_KEY: &str = "@sheet.paper";
/// A standard size's orientation, in construct-params' stored choice text.
pub const ORIENTATION_KEY: &str = "@sheet.orientation";
/// A custom paper's width, a stored length.
pub const WIDTH_KEY: &str = "@sheet.width";
/// A custom paper's height, a stored length.
pub const HEIGHT_KEY: &str = "@sheet.height";
/// The longest a sheet number may be, in characters.
pub const MAX_NUMBER_LEN: usize = 32;

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a sheet record: Spare Construct's own operation `sheet`, version 1, with no
/// arguments. It builds no shape.
#[must_use]
pub fn sheet_recipe() -> Recipe {
    Recipe::construct(SHEET_KIND, BTreeMap::new())
}

/// A sheet number as kept: trimmed, not empty, short, with no control character.
pub(crate) fn clean_number(number: &str) -> Result<String, SheetError> {
    let number = number.trim();
    if number.is_empty() {
        return Err(SheetError::EmptyNumber);
    }
    if number.chars().count() > MAX_NUMBER_LEN || number.chars().any(char::is_control) {
        return Err(SheetError::BadNumber(format!(
            "A sheet number is at most {MAX_NUMBER_LEN} characters with no control characters."
        )));
    }
    Ok(number.to_owned())
}

/// A sheet: a number, a name and a paper size. Its number is kept unique in the document by
/// [`crate::Sheets`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    id: Id,
    number: String,
    name: String,
    paper: Paper,
}

impl Sheet {
    /// A sheet.
    ///
    /// # Errors
    /// [`SheetError::EmptyName`], [`SheetError::EmptyNumber`], [`SheetError::BadNumber`] or
    /// [`SheetError::BadPaper`].
    pub fn new(id: Id, number: &str, name: &str, paper: Paper) -> Result<Self, SheetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SheetError::EmptyName);
        }
        paper.check()?;
        Ok(Self {
            id,
            number: clean_number(number)?,
            name: name.to_owned(),
            paper,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its number, such as `A101`.
    #[must_use]
    pub fn number(&self) -> &str {
        &self.number
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its paper.
    #[must_use]
    pub fn paper(&self) -> &Paper {
        &self.paper
    }

    /// The sheet with another number.
    ///
    /// # Errors
    /// [`SheetError::EmptyNumber`] or [`SheetError::BadNumber`].
    pub fn with_number(mut self, number: &str) -> Result<Self, SheetError> {
        self.number = clean_number(number)?;
        Ok(self)
    }

    /// The sheet with another name.
    ///
    /// # Errors
    /// [`SheetError::EmptyName`].
    pub fn with_name(mut self, name: &str) -> Result<Self, SheetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SheetError::EmptyName);
        }
        name.clone_into(&mut self.name);
        Ok(self)
    }

    /// The sheet on other paper.
    ///
    /// # Errors
    /// [`SheetError::BadPaper`].
    pub fn with_paper(mut self, paper: Paper) -> Result<Self, SheetError> {
        paper.check()?;
        self.paper = paper;
        Ok(self)
    }

    /// The sheet as an element record of `document` (FORMAT.md beside this crate), with no
    /// parameter values: a command that rewrites a sheet keeps the ones the record had
    /// ([`Sheet::over`]).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), SHEETS.0.to_string());
        props.insert(NUMBER_KEY.to_owned(), self.number.clone());
        props.insert(
            PAPER_KEY.to_owned(),
            Value::Choice(self.paper.key().to_owned()).stored(),
        );
        match self.paper {
            Paper::Standard { orientation, .. } => {
                props.insert(
                    ORIENTATION_KEY.to_owned(),
                    Value::Choice(orientation.key().to_owned()).stored(),
                );
            }
            Paper::Custom { width, height } => {
                props.insert(WIDTH_KEY.to_owned(), Value::Length(width).stored());
                props.insert(HEIGHT_KEY.to_owned(), Value::Length(height).stored());
            }
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: SHEET_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: sheet_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// The record of the sheet written over `old`, keeping every entry of `old` that is not one
    /// of the sheet's own (the parameter values).
    #[must_use]
    pub fn over(&self, old: &Element) -> Element {
        let mut record = self.to_record(&old.document);
        for (key, value) in &old.props {
            if !key.starts_with('@') {
                record.props.insert(key.clone(), value.clone());
            }
        }
        record
    }

    /// Reads a sheet from its element record. Anything [`Sheet::to_record`] would not write is
    /// refused, never repaired, except entries whose keys do not start with `@`: those are
    /// parameter values and are left to construct-params.
    ///
    /// # Errors
    /// [`SheetError::Malformed`], naming the record, for a record that is not a sheet record as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, SheetError> {
        let malformed =
            |why: &str| SheetError::Malformed(format!("The sheet record {} {why}.", record.id));
        if record.kind != SHEET_KIND || record.recipe != sheet_recipe() {
            return Err(malformed("is not of kind sheet, with the sheet recipe"));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let number = record
            .props
            .get(NUMBER_KEY)
            .ok_or_else(|| malformed("holds no number"))?;
        let Some(Value::Choice(paper_key)) = stored(record, PAPER_KEY) else {
            return Err(malformed("holds no paper in its one spelling"));
        };
        let paper = if paper_key == "custom" {
            let (Some(width), Some(height)) =
                (length(record, WIDTH_KEY), length(record, HEIGHT_KEY))
            else {
                return Err(malformed("holds a custom paper with no width and height"));
            };
            Paper::Custom { width, height }
        } else {
            let Some(size) = PaperSize::from_key(&paper_key) else {
                return Err(malformed("holds a paper size that is not one of ours"));
            };
            let Some(Value::Choice(orientation)) = stored(record, ORIENTATION_KEY) else {
                return Err(malformed("holds no orientation in its one spelling"));
            };
            let Some(orientation) = Orientation::from_key(&orientation) else {
                return Err(malformed(
                    "holds an orientation that is not landscape or portrait",
                ));
            };
            Paper::Standard { size, orientation }
        };
        let sheet = Self::new(record.id.clone(), number, &record.name, paper)
            .map_err(|e| malformed(&e.to_string()))?;
        // Compare as written, with the parameter values left out.
        let mut bare = record.clone();
        bare.props.retain(|key, _| key.starts_with('@'));
        if sheet.to_record(&record.document) != bare {
            return Err(malformed("is not as a sheet record is written"));
        }
        Ok(sheet)
    }
}

/// A stored value under `key`, if it is in its one spelling.
pub(crate) fn stored(record: &Element, key: &str) -> Option<Value> {
    let text = record.props.get(key)?;
    Value::from_stored(text).filter(|value| value.stored() == *text)
}

/// A stored length under `key`, if it is in its one spelling.
pub(crate) fn length(record: &Element, key: &str) -> Option<Length> {
    match stored(record, key)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}
