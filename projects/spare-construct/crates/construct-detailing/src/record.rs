//! What every detail record shares: how a length, a point and the view are held in its `props`,
//! and how a record is built and read back. Every spelling here is the one construct-params gives
//! (`Value::stored`), and a reader refuses a record that is not exactly as the writer would make it.

use crate::error::DetailError;
use construct_grids::Point;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The view an item is in, an ID. Every detail item belongs to exactly one view, and is shown in
/// no other.
pub const VIEW_KEY: &str = "@detail.view";
/// The record's category, in its one spelling.
pub const CATEGORY_KEY: &str = "@category";

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The properties being written for one record.
pub(crate) struct Props(pub(crate) BTreeMap<String, String>);

impl Props {
    pub(crate) fn new(category: u32, view: &Id) -> Self {
        let mut map = BTreeMap::new();
        map.insert(CATEGORY_KEY.to_owned(), category.to_string());
        map.insert(VIEW_KEY.to_owned(), view.to_string());
        Self(map)
    }

    pub(crate) fn length(&mut self, key: &str, length: Length) {
        self.0
            .insert(key.to_owned(), Value::Length(length).stored());
    }

    pub(crate) fn point(&mut self, key: &str, p: Point) {
        self.length(&format!("{key}.x"), p.x);
        self.length(&format!("{key}.y"), p.y);
    }

    pub(crate) fn text(&mut self, key: &str, text: &str) {
        self.0.insert(key.to_owned(), text.to_owned());
    }

    pub(crate) fn finish(
        self,
        id: &Id,
        document: &Id,
        kind: &str,
        name: &str,
        references: Vec<construct_model::element::Reference>,
    ) -> Element {
        Element {
            id: id.clone(),
            document: document.clone(),
            kind: kind.to_owned(),
            name: name.to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: Recipe::construct(kind, BTreeMap::new()),
            props: self.0,
            references,
        }
    }
}

/// A record being read.
pub(crate) struct Reader<'a> {
    pub(crate) record: &'a Element,
    what: &'a str,
}

impl<'a> Reader<'a> {
    /// Starts reading `record`, which must be of `kind`, with no level or placement.
    pub(crate) fn new(record: &'a Element, kind: &str, what: &'a str) -> Result<Self, DetailError> {
        let reader = Self { record, what };
        if record.kind != kind || record.recipe != Recipe::construct(kind, BTreeMap::new()) {
            return Err(reader.bad(&format!("is not of kind {kind}, with its recipe")));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT {
            return Err(reader.bad("has a level or a placement"));
        }
        Ok(reader)
    }

    pub(crate) fn bad(&self, why: &str) -> DetailError {
        DetailError::Malformed(format!(
            "The {} record {} {why}.",
            self.what, self.record.id
        ))
    }

    pub(crate) fn text(&self, key: &str) -> Result<&'a str, DetailError> {
        self.record
            .props
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| self.bad(&format!("holds no {key}")))
    }

    pub(crate) fn view(&self) -> Result<Id, DetailError> {
        Id::parse(self.text(VIEW_KEY)?).map_err(|_| self.bad("holds no view ID"))
    }

    pub(crate) fn length(&self, key: &str) -> Result<Length, DetailError> {
        let text = self.text(key)?;
        match Value::from_stored(text).filter(|v| v.stored() == text) {
            Some(Value::Length(length)) => Ok(length),
            _ => Err(self.bad(&format!("holds {key} that is not in its one spelling"))),
        }
    }

    pub(crate) fn point(&self, key: &str) -> Result<Point, DetailError> {
        Ok(Point::new(
            self.length(&format!("{key}.x"))?,
            self.length(&format!("{key}.y"))?,
        ))
    }

    /// Checks the record is exactly what `written` would give, which is the one spelling.
    pub(crate) fn same_as(&self, written: &Element) -> Result<(), DetailError> {
        if written == self.record {
            Ok(())
        } else {
            Err(self.bad(&format!("is not as a {} record is written", self.what)))
        }
    }
}
